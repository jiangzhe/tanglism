// 定义K线相关函数
// 依赖jquery, jquery-ui, d3, tanglism-common

export const kline = {
    data,
    conf,
    draw,
    add_draw_callback,
    clear_drawing,
    ajax,
    add_data_callback
};

// k线数据
const _data = [];
// 画图回调
const draw_callbacks = [];
// 数据回调
const data_callbacks = [];

// 获取提示框，若不存在则创建
export function tooltip() {
  var t = d3.select("#k_container div.tooltip");
  if (!t.empty()) {
    return t;
  }
  return d3.select("#k_container")
    .append("div")
    .attr("class", "tooltip")
    .style("opacity", 0);
}

// k线基础配置
function conf() {
    // 单柱宽度，包含间隔
    var bar_width = parseFloat($("#bar_width").val());
    // 单柱间间隔
    var bar_padding;
    if ($("#bar_padding_fixed").is(":checked")) {
      bar_padding = parseFloat($("#bar_padding_fixed_width").val());
    } else {
      bar_padding = Math.max(bar_width / 3, 4);
    }
    // 柱内宽度，即显示出的红/绿柱宽度
    var bar_inner_width = bar_width - bar_padding;
    // 整体宽度
    var w = bar_width * kline.data().length;
    // 整体高度
    var h = parseInt($("#chart_height").val());
    // 价格最大值
    var max_value = d3.max(kline.data(), function(d) {
      return d.high;
    });
    // 价格最小值
    var min_value = d3.min(kline.data(), function(d) {
        return d.low;
    });
    // 缩放比例
    var yscale = d3.scaleLinear([min_value, max_value], [0, h]);
    return {
      bar_width,
      bar_padding,
      bar_inner_width,
      w,
      h,
      yscale
    };
};

// data函数，无参调用返回数据，有参调用刷新数据
function data(input) {
    if (input) {
        while(_data.length > 0) { _data.pop(); }
        for (var i = 0; i < input.length; i++) {
            _data.push(input[i]);
        }
        return;
    }
    return _data;
}

// draw函数
function draw() {
    var conf = kline.conf();
    // 创建K线图
    if (!d3.select("#k_lines").empty()) {
      // 如存在则删除
      d3.select("#k_lines").remove();
    }
    var svg = d3.select("#k_container")
      .append("svg")
      .attr("id", "k_lines")
      .attr("width", conf.w)
      .attr("height", conf.h);
    // 画图
    svg.selectAll("rect").data(kline.data()).enter().append("rect")
        .attr("x", function(d, i) {
            return i * conf.bar_width;
        })
        .attr('y', function(d, i) {
            return conf.h - conf.yscale(d3.max([d.open, d.close]));
        })
        .attr('width', conf.bar_inner_width)
        .attr("height", function(d) {
          // 当开盘与收盘价相等时，至少保证1的高度
          return Math.max(1, Math.abs(conf.yscale(d.open) - conf.yscale(d.close)));
        })
        .attr("fill", function(d) {
            if (d.open < d.close) return "red";
            return "green";
        })
        .on("mouseover", function(d) {
          tooltip().transition()
            .duration(200)
            .style("opacity", 0.9);
          var dt = d.ts.substring(0, 10);
          var tm = d.ts.substring(11, 16);
          tooltip()
            .html(
              "日期：" + dt + "<br/>" + 
              "时刻：" + tm + "<br/>" +
              "开盘价：" + d.open + "<br/>" +
              "收盘价：" + d.close + "<br/>" +
              "最高价：" + d.high + "<br/>" + 
              "最低价：" + d.low)
            .style("left", (d3.event.pageX + 30) + "px")
            .style("top", (d3.event.pageY + 30) + "px");
        })
        .on("mouseout", function(d) {
          tooltip().transition()
            .duration(500)
            .style("opacity", 0);
        });
          
    // 构造中线
    svg.selectAll("line.k")
    .data(kline.data())
    .enter()
    .append("line")
    .attr("class", "k")
    .attr("x1", function(d, i) {
        return i * conf.bar_width + conf.bar_inner_width / 2;
    })
    .attr("x2", function(d, i) {
        return i * conf.bar_width + conf.bar_inner_width / 2;
    })
    .attr("y1", function(d) {
        return conf.h - conf.yscale(d.high);
    })
    .attr("y2", function(d) {
        return conf.h - conf.yscale(d.low);
    })
    .attr("stroke", function(d) {
        if (d.open < d.close) return "red";
        return "green";
    });

    for (var i = 0; i < draw_callbacks.length; i++) {
        draw_callbacks[i](conf);
    }
    // stroke_draw(conf);
    // segment_draw(conf);
    // subtrend_draw(conf);
};

function clear_drawing() {
    // 删除tooltip
    d3.select("#k_tooltip").remove();
    // 删除svg
    d3.select("#k_lines").remove();
}

function add_draw_callback(callback) {
    draw_callbacks.push(callback);
}

function ajax(params) {
    $.ajax({
        url: "api/v1/stock-prices/" + encodeURIComponent(params.code)
          + "/ticks/" + encodeURIComponent(params.tick) + "?start_dt="
          + encodeURIComponent(params.start_dt) + "&end_dt="
          + encodeURIComponent(params.end_dt),
        method: "GET",
        dataType: "json",
        success: function(resp) {
          var kdata = $.map(resp.data, function(item) {
            return {
              ts: item.ts,
              open: parseFloat(item.open),
              close: parseFloat(item.close),
              high: parseFloat(item.high),
              low: parseFloat(item.low)
            };
          });
          kline.data(kdata);
          for (var i=0; i<data_callbacks.length;i++) {
            data_callbacks[i](kdata);
          }
          kline.draw();
        },
        error: function(err) {
          console.log("ajax error on query prices", err);
          kline.clear_drawing();
        }
      })
}

function add_data_callback(callback) {
    data_callbacks.push(callback);
}
