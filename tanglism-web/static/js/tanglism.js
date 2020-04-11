var charts = (function(){
  // K线数据
  var kdata = [];
  // 分型数据
  var pdata = [];
  // 笔数据
  var skdata = [];
  // 线段数据
  var sgdata = [];
  return {
    kdata: function(input) {
      if (input) {
        while(kdata.length > 0) { kdata.pop(); }
        for (var i = 0; i < input.length; i++) {
          kdata.push(input[i]);
        }
        return;
      }
      return kdata;
    },
    kdata_draw: function() {
      // 单柱宽度，包含间隔
      var bar_width = parseInt($("#bar_width option:selected").text());
      var bar_padding = Math.max(bar_width / 3, 4);
      var bar_inner_width = bar_width - bar_padding;
      // 整体宽度
      var w = bar_width * kdata.length;
      // 整体高度
      var h = parseInt($("#chart_height option:selected").text());
      // 最大最小值统计
      var price_max = d3.max(kdata, function(d) {
        return d.high;
      });
      var price_min = d3.min(kdata, function(d) {
          return d.low;
      });
      // 缩放比例
      var yscale = d3.scaleLinear([price_min, price_max], [0, h]);
      // 构造标题
      if (!d3.select("#k_lines_title").empty()) {
        d3.select("#k_lines_title").remove();
      }
      d3.select("#k_container").append("div").attr("id", "k_lines_title")
        .text("K线图");
      // 构造单例K线图
      if (!d3.select("#k_lines").empty()) {
        // 如存在则删除
        d3.select("#k_lines").remove();
      }
      var svg = d3.select("#k_container")
        .append("svg")
        .attr("id", "k_lines")
        .attr("width", w)
        .attr("height", h);
      // 构造单例提示
      if (!d3.select("#k_tooltip").empty()) {
        d3.select("#k_tooltip").remove();
      }
      var tooltip = d3.select("#k_container")
        .append("div")
        .attr("class", "tooltip")
        .style("opacity", 0);

      // 构造柱状图
      svg.selectAll("rect").data(kdata).enter().append("rect")
          .attr("x", function(d, i) {
              return i * bar_width;
          })
          .attr('y', function(d, i) {
              return h - yscale(d3.max([d.open, d.close]));
          })
          .attr('width', bar_inner_width)
          .attr("height", function(d) {
            // 当开盘与收盘价相等时，至少保证1的高度
            return Math.max(1, Math.abs(yscale(d.open) - yscale(d.close)));
          })
          .attr("fill", function(d) {
              if (d.open < d.close) return "red";
              return "green";
          })
          .on("mouseover", function(d) {
            tooltip.transition()
              .duration(200)
              .style("opacity", 0.9);
            var dt = d.date.substring(0, 10);
            var tm = d.date.substring(11, 16);
            tooltip
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
            tooltip.transition()
              .duration(500)
              .style("opacity", 0);
          });
    
      // 构造中线
      svg.selectAll("line")
          .data(kdata)
          .enter()
          .append("line")
          .attr("x1", function(d, i) {
              return i * bar_width + bar_inner_width / 2;
          })
          .attr("x2", function(d, i) {
              return i * bar_width + bar_inner_width / 2;
          })
          .attr("y1", function(d) {
              return h - yscale(d.high);
          })
          .attr("y2", function(d) {
              return h - yscale(d.low);
          })
          .attr("stroke", function(d) {
              if (d.open < d.close) return "red";
              return "green";
          });
    },
    kdata_clear: function() {
      // 删除tooltip
      d3.select("#k_tooltip").remove();
      // 删除svg
      d3.select("#k_lines").remove();
      // 删除标题
      d3.select("#k_lines_title").remove();
      // 删除数据
      while(kdata.length > 0) { kdata.pop(); }
    },
    pdata: function(input) {
      if (input) {
        while(pdata.length > 0) { pdata.pop(); }
        for (var i = 0; i < input.length; i++) {
          pdata.push(input[i]);
        }
        return;
      }
      return pdata;
    }
  };
})();

$(document).ready(function() {
  $("#input_stock_code").autocomplete({
    source: function(req, callback) {
      $.ajax({
        url: "api/v1/keyword-stocks?keyword=" + encodeURIComponent(req.term),
        method: "GET",
        dataType: "json",
        success: function(resp) {
          callback($.map(resp.content, function(item){
            return {
              value: item.code,
              label: item.code + " " + item.display_name
            };
          }));
        },
        error: function(err) {
          console.log("ajax error on search stock", err);
          callback([]);
        }
      })
    }
  });
  $("#input_start_dt").datepicker({
    dateFormat: "yy-mm-dd",
    minDate: "2010-01-01",
    maxDate: -1
  });
  $("#input_end_dt").datepicker({
    dateFormat: "yy-mm-dd",
    minDate: "2010-01-01",
    maxDate: -1
  });
  $("#stock_submission").click(function() {
    var input_stock_code = $("#input_stock_code").val();
    var input_start_dt = $("#input_start_dt").val();
    var input_end_dt = $("#input_end_dt").val();
    var input_tick = $("#input_tick option:selected").text();
    $.ajax({
      url: "api/v1/stock-prices/" + encodeURIComponent(input_stock_code)
        + "/ticks/" + encodeURIComponent(input_tick) + "?start_dt="
        + encodeURIComponent(input_start_dt) + "&end_dt="
        + encodeURIComponent(input_end_dt),
      method: "GET",
      dataType: "json",
      success: function(resp) {
        var kdata = $.map(resp.data, function(item) {
          return {
            date: item.ts,
            open: parseFloat(item.open),
            close: parseFloat(item.close),
            high: parseFloat(item.high),
            low: parseFloat(item.low)
          };
        });
        charts.kdata(kdata);
        charts.kdata_draw();
      },
      error: function(err) {
        console.log("ajax error on query prices", err);
        charts.kdata_clear();
      }
    })
  });
  $("#chart_refresh").click(function() {
    charts.kdata_draw();
  });

});

