var charts = (function(){
  // K线数据
  var kdata = [];
  // 分型数据
  var pdata = [];
  // 笔数据
  var skdata = [];
  // 线段数据
  var sgdata = [];

  // k线基础配置
  var kconf = function() {
    // 单柱宽度，包含间隔
    var bar_width = parseInt($("#bar_width option:selected").text());
    // 单柱间间隔
    var bar_padding = Math.max(bar_width / 3, 4);
    // 柱内宽度，即显示出的红/绿柱宽度
    var bar_inner_width = bar_width - bar_padding;
    // 整体宽度
    var w = bar_width * kdata.length;
    // 整体高度
    var h = parseInt($("#chart_height option:selected").text());
    // 价格最大值
    var price_max = d3.max(kdata, function(d) {
      return d.high;
    });
    // 价格最小值
    var price_min = d3.min(kdata, function(d) {
        return d.low;
    });
    // 缩放比例
    var yscale = d3.scaleLinear([price_min, price_max], [0, h]);

    return {
      bar_width,
      bar_padding,
      bar_inner_width,
      w,
      h,
      price_max,
      price_min,
      yscale
    };
  };
  var kdata_fn = function(input) {
    if (input) {
      while(kdata.length > 0) { kdata.pop(); }
      for (var i = 0; i < input.length; i++) {
        kdata.push(input[i]);
      }
      return;
    }
    return kdata;
  };
  var kdata_clear = function() {
    // 删除tooltip
    d3.select("#k_tooltip").remove();
    // 删除svg
    d3.select("#k_lines").remove();
    // 删除标题
    d3.select("#k_lines_title").remove();
  };
  // 仅在K线图完成后调用
  var stroke_draw = function(config) {
    var stroke_draw_check = $("#stroke_draw").is(":checked");
    if (!stroke_draw_check) {
      return;
    }
    var conf = config || kconf();
    // 无K线图，直接退出
    if (d3.select("#k_lines").empty()) {
      return;
    }
    // 无K线数据或笔数据，直接退出
    if (kdata.length == 0 || skdata.length == 0) {
      return;
    }
    // 双指针，查询笔所在柱状图，并添加序列号
    var ki = 0;
    var si = 0;
    while (si < skdata.length && ki < kdata.length) {
      var sk = skdata[si];
      var k = kdata[ki];
      if (sk.start_pt.extremum_ts === k.ts) {
        // 起点序列号
        sk.start_id = ki;
        // 递增笔
        ki++;
      } else if (sk.end_pt.extremum_ts === k.ts) {
        // 终点序列号
        sk.end_id = ki;
        // 仅递增笔，下一笔起点应与前一笔终点一致，需复用ki
        si++;
      } else {
        // 未匹配到，K线号递增
        ki++;
      }
    }

    // 过滤出所有匹配上的笔
    var strokes = [];
    for (var i = 0; i < skdata.length; i++) {
      var item = skdata[i];
      if (item.hasOwnProperty("start_id") && item.hasOwnProperty("end_id")) {
        strokes.push(item);
      }
    }
    var svg = d3.select("#k_lines");
    svg.selectAll("line.stroke")
        .data(strokes)
        .enter()
        .append("line")
        .attr("class", "stroke")
        .attr("x1", function(d, i) {
            return d.start_id * conf.bar_width + conf.bar_inner_width / 2;
        })
        .attr("x2", function(d, i) {
            return d.end_id * conf.bar_width + conf.bar_inner_width / 2;
        })
        .attr("y1", function(d) {
            return conf.h - conf.yscale(parseFloat(d.start_pt.extremum_price));
        })
        .attr("y2", function(d) {
            return conf.h - conf.yscale(parseFloat(d.end_pt.extremum_price));
        })
        .attr("stroke", "blue");
  }

  var segment_draw = function(config) {
    // 是否在图中显示线段
    var segment_draw_check = $("#segment_draw").is(":checked");
    if (!segment_draw_check) {
      return;
    }
    var conf = config || kconf();
    // 无K线图，直接退出
    if (d3.select("#k_lines").empty()) {
      return;
    }
    // 无K线数据或线段数据，直接退出
    if (kdata.length == 0 || sgdata.length == 0) {
      return;
    }
    // 双指针，查询笔所在柱状图，并添加序列号
    var ki = 0;
    var si = 0;
    while (si < sgdata.length && ki < kdata.length) {
      var sg = sgdata[si];
      var k = kdata[ki];
      if (sg.start_pt.extremum_ts === k.ts) {
        // 起点序列号
        sg.start_id = ki;
        // 递增线段
        ki++;
      } else if (sg.end_pt.extremum_ts === k.ts) {
        // 终点序列号
        sg.end_id = ki;
        // 仅递增线段，下一线段起点应与前一线段终点一致，需复用ki
        si++;
      } else {
        // 未匹配到，K线号递增
        ki++;
      }
    }

    // 过滤出所有匹配上的线段
    var segments = [];
    for (var i = 0; i < sgdata.length; i++) {
      var item = sgdata[i];
      if (item.hasOwnProperty("start_id") && item.hasOwnProperty("end_id")) {
        segments.push(item);
      }
    }
    var svg = d3.select("#k_lines");
    svg.selectAll("line.segment")
        .data(segments)
        .enter()
        .append("line")
        .attr("class", "segment")
        .attr("x1", function(d, i) {
            return d.start_id * conf.bar_width + conf.bar_inner_width / 2;
        })
        .attr("x2", function(d, i) {
            return d.end_id * conf.bar_width + conf.bar_inner_width / 2;
        })
        .attr("y1", function(d) {
            return conf.h - conf.yscale(parseFloat(d.start_pt.extremum_price));
        })
        .attr("y2", function(d) {
            return conf.h - conf.yscale(parseFloat(d.end_pt.extremum_price));
        })
        .attr("stroke", "black");
  }

  var kdata_draw = function() {
    var conf = kconf();
    // 创建标题
    if (!d3.select("#k_lines_title").empty()) {
      d3.select("#k_lines_title").remove();
    }
    d3.select("#k_container").append("div").attr("id", "k_lines_title")
      .text("K线图");
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
    // 创建单例提示
    if (!d3.select("#k_tooltip").empty()) {
      d3.select("#k_tooltip").remove();
    }
    var tooltip = d3.select("#k_container")
      .append("div")
      .attr("class", "tooltip")
      .style("opacity", 0);
    // 画图
    svg.selectAll("rect").data(kdata).enter().append("rect")
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
          tooltip.transition()
            .duration(200)
            .style("opacity", 0.9);
          var dt = d.ts.substring(0, 10);
          var tm = d.ts.substring(11, 16);
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
    svg.selectAll("line.k")
        .data(kdata)
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

    stroke_draw(conf);
    segment_draw(conf);
  };
  var pdata_fn = function(input) {
    if (input) {
      while(pdata.length > 0) { pdata.pop(); }
      for (var i = 0; i < input.length; i++) {
        pdata.push(input[i]);
      }
      return;
    }
    return pdata;
  };
  var pdata_clear = function() {
    // 删除表格
    d3.select("#p_table").remove();
    // 删除标题
    d3.select("#p_table_title").remove();
  };
  var pdata_table = function() {
    var p_check = $("#p_check").is(":checked");
    if (!p_check) {
      pdata_clear();
      return;
    }
    // 创建标题
    if (!d3.select("#p_table_title").empty()) {
      d3.select("#p_table_title").remove();
    }
    d3.select("#p_container").append("div")
      .attr("id", "p_table_title")
      .text("分型");
    // 创建表格
    if (!d3.select("#p_table").empty()) {
      d3.select("#p_table").remove();
    }
    var table = d3.select("#p_container").append("table")
      .attr("id", "p_table")
      .style("border-collapse", "collapse")
      .style("border", "2px black solid");
    // 表头
    table.append("thead")
      .append("tr")
      .selectAll("th")
      .data(["峰值时刻", "峰值价格", "起始时刻", "结束时刻", "K线数目", "类型"])
      .enter()
      .append("th")
      .text(function(d) {return d;})
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("background-color", "lightgray")
      .style("font-weight", "bold");
    // 内容
    table.append("tbody")
      .selectAll("tr")
      .data(pdata)
      .enter()
      .append("tr")
      .selectAll("td")
      .data(function(d) {
        return [d.extremum_ts, d.extremum_price, d.start_ts, d.end_ts, d.n, d.top ? "顶分型" : "底分型"];
      })
      .enter()
      .append("td")
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("font-size", "12px")
      .text(function(d) {return d;})
      .on("mouseover", function(){
        d3.select(this).style("background-color", "powderblue");
      })
      .on("mouseout", function(){
        d3.select(this).style("background-color", "white");
      });
  };
  var skdata_fn = function(input) {
    if (input) {
      while(skdata.length > 0) { skdata.pop(); }
      for (var i = 0; i < input.length; i++) {
        skdata.push(input[i]);
      }
      return;
    }
    return skdata;
  };
  var skdata_clear = function() {
    // 删除表格
    d3.select("#sk_table").remove();
    // 删除标题
    d3.select("#sk_table_title").remove();
  };
  var skdata_table = function() {
    var sk_check = $("#sk_check").is(":checked");
    if (!sk_check) {
      skdata_clear();
      return;
    }
    // 创建标题
    if (!d3.select("#sk_table_title").empty()) {
      d3.select("#sk_table_title").remove();
    }
    d3.select("#sk_container").append("div")
      .attr("id", "sk_table_title")
      .text("笔");
    // 创建表格
    if (!d3.select("#sk_table").empty()) {
      d3.select("#sk_table").remove();
    }
    var table = d3.select("#sk_container").append("table")
      .attr("id", "sk_table")
      .style("border-collapse", "collapse")
      .style("border", "2px black solid");
    // 表头
    table.append("thead")
      .append("tr")
      .selectAll("th")
      .data(["起始时刻", "起始价格", "终止时刻", "终止价格", "走向"])
      .enter()
      .append("th")
      .text(function(d) {return d;})
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("background-color", "lightgray")
      .style("font-weight", "bold");
    // 内容
    table.append("tbody")
      .selectAll("tr")
      .data(skdata)
      .enter()
      .append("tr")
      .selectAll("td")
      .data(function(d) {
        return [
          d.start_pt.extremum_ts, 
          d.start_pt.extremum_price, 
          d.end_pt.extremum_ts, 
          d.end_pt.extremum_price, 
          parseFloat(d.start_pt.extremum_price) < parseFloat(d.end_pt.extremum_price) ? "上升" : "下降"
        ];
      })
      .enter()
      .append("td")
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("font-size", "12px")
      .text(function(d) {return d;})
      .on("mouseover", function(){
        d3.select(this).style("background-color", "powderblue");
      })
      .on("mouseout", function(){
        d3.select(this).style("background-color", "white");
      });
  };
  var sgdata_fn = function(input) {
    if (input) {
      while(sgdata.length > 0) { sgdata.pop(); }
      for (var i = 0; i < input.length; i++) {
        sgdata.push(input[i]);
      }
      return;
    }
    return sgdata;
  };
  var sgdata_clear = function() {
    // 删除表格
    d3.select("#sg_table").remove();
    // 删除标题
    d3.select("#sg_table_title").remove();
  };
  var sgdata_table = function() {
    var sg_check = $("#sg_check").is(":checked");
    if (!sg_check) {
      sgdata_clear();
      return;
    }
    // 创建标题
    if (!d3.select("#sg_table_title").empty()) {
      d3.select("#sg_table_title").remove();
    }
    d3.select("#sg_container").append("div")
      .attr("id", "sg_table_title")
      .text("线段");
    // 创建表格
    if (!d3.select("#sg_table").empty()) {
      d3.select("#sg_table").remove();
    }
    var table = d3.select("#sg_container").append("table")
      .attr("id", "sg_table")
      .style("border-collapse", "collapse")
      .style("border", "2px black solid");
    // 表头
    table.append("thead")
      .append("tr")
      .selectAll("th")
      .data(["起始时刻", "起始价格", "终止时刻", "终止价格", "走向"])
      .enter()
      .append("th")
      .text(function(d) {return d;})
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("background-color", "lightgray")
      .style("font-weight", "bold");
    // 内容
    table.append("tbody")
      .selectAll("tr")
      .data(sgdata)
      .enter()
      .append("tr")
      .selectAll("td")
      .data(function(d) {
        return [
          d.start_pt.extremum_ts, 
          d.start_pt.extremum_price, 
          d.end_pt.extremum_ts, 
          d.end_pt.extremum_price, 
          parseFloat(d.start_pt.extremum_price) < parseFloat(d.end_pt.extremum_price) ? "上升" : "下降"
        ];
      })
      .enter()
      .append("td")
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("font-size", "12px")
      .text(function(d) {return d;})
      .on("mouseover", function(){
        d3.select(this).style("background-color", "powderblue");
      })
      .on("mouseout", function(){
        d3.select(this).style("background-color", "white");
      });
  };
  var clear = function() {
    while(kdata.length > 0) { kdata.pop(); }
    while(pdata.length > 0) { pdata.pop(); }
    while(skdata.length > 0) { skdata.pop(); }
    while(sgdata.length > 0) { sgdata.pop(); }
  }

  return {
    kdata: kdata_fn,
    kdata_draw,
    stroke_draw,
    segment_draw,
    kdata_clear,
    pdata: pdata_fn,
    pdata_table,
    pdata_clear,
    skdata: skdata_fn,
    skdata_table,
    skdata_clear,
    sgdata: sgdata_fn,
    sgdata_table,
    sgdata_clear,
    clear
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
  var pdata_ajax = function(tick, code, start_dt, end_dt) {
    $.ajax({
      url: "api/v1/tanglism/partings/" + encodeURIComponent(code)
        + "/ticks/" + encodeURIComponent(tick) 
        + "?start_dt=" + encodeURIComponent(start_dt) 
        + "&end_dt=" + encodeURIComponent(end_dt),
      method: "GET",
      dataType: "json",
      success: function(resp) {
        charts.pdata(resp.data);
        charts.pdata_table();
      },
      error: function(err) {
        console.log("ajax error on query partings", err);
        charts.pdata_clear();
      }
    });
  };

  var skdata_ajax = function(tick, code, start_dt, end_dt, indep_k_check) {
    $.ajax({
      url: "api/v1/tanglism/strokes/" + encodeURIComponent(code)
        + "/ticks/" + encodeURIComponent(tick) 
        + "?start_dt=" + encodeURIComponent(start_dt) 
        + "&end_dt=" + encodeURIComponent(end_dt)
        + "&indep_k=" + encodeURIComponent(indep_k_check),
      method: "GET",
      dataType: "json",
      success: function(resp) {
        charts.skdata(resp.data),
        charts.skdata_table();
        charts.stroke_draw();
      },
      error: function(err) {
        console.log("ajax error on query strokes", err);
        charts.skdata_clear();
      }
    });
  };

  var sgdata_ajax = function(tick, code, start_dt, end_dt, indep_k_check) {
    $.ajax({
      url: "api/v1/tanglism/segments/" + encodeURIComponent(code)
        + "/ticks/" + encodeURIComponent(tick) 
        + "?start_dt=" + encodeURIComponent(start_dt) 
        + "&end_dt=" + encodeURIComponent(end_dt)
        + "&indep_k=" + encodeURIComponent(indep_k_check),
      method: "GET",
      dataType: "json",
      success: function(resp) {
        charts.sgdata(resp.data),
        charts.sgdata_table();
        charts.segment_draw();
      },
      error: function(err) {
        console.log("ajax error on query strokes", err);
        charts.sgdata_clear();
      }
    });
  }

  $("#stock_submission").click(function() {
    var input_stock_code = $("#input_stock_code").val();
    var input_start_dt = $("#input_start_dt").val();
    var input_end_dt = $("#input_end_dt").val();
    var input_tick = $("#input_tick option:selected").text();
    
    var p_check = $("#p_check").is(":checked");
    var sk_check = $("#sk_check").is(":checked");
    var sg_check = $("#sg_check").is(":checked");
    var indep_k_check = $("#indep_k_check").is(":checked");
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
            ts: item.ts,
            open: parseFloat(item.open),
            close: parseFloat(item.close),
            high: parseFloat(item.high),
            low: parseFloat(item.low)
          };
        });
        // 清除所有数据
        charts.clear();
        charts.kdata(kdata);
        charts.kdata_draw();
        if (p_check) {
          pdata_ajax(input_tick, input_stock_code, input_start_dt, input_end_dt);
        }
        if (sk_check) {
          skdata_ajax(input_tick, input_stock_code, input_start_dt, input_end_dt, indep_k_check);
        }
        if (sg_check) {
          sgdata_ajax(input_tick, input_stock_code, input_start_dt, input_end_dt, indep_k_check);
        }
      },
      error: function(err) {
        console.log("ajax error on query prices", err);
        charts.kdata_clear();
      }
    })
  });
  $("#chart_refresh").click(function() {
    charts.kdata_draw();
    charts.pdata_table();
    charts.skdata_table();
    charts.sgdata_table();
  });

});

