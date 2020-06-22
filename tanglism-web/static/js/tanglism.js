import { setupWebsocketEvents, validate_basic_cfg, draw } from './tanglism-draw.js';
import { atr } from './atr.js';

$(document).ready(function() {
  // 股票选择
  $("#input_stock_code").autocomplete({
    source: function(req, callback) {
      $.ajax({
        url: "/api/keyword-stocks?keyword=" + encodeURIComponent(req.term),
        method: "GET",
        dataType: "json",
        success: function(resp) {
          callback($.map(resp, function(item){
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
  // 时间选择
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
  $("#data_container").tabs();
  // 笔逻辑选择
  $("input[name='stroke_logic_gap']").click(function(e){
    var value = $(this).val();
    if (value === "gap_ratio") {
      $("#gap_ratio_percentage_span").css("display", "inline");
    } else {
      $("#gap_ratio_percentage_span").css("display", "none");
    }
  });
  // 柱间距选择
  $("input[name='bar_padding']").click(function(e){
    var value = $(this).val();
    if (value === "fixed") {
      $("#bar_padding_fixed_width_span").css("display", "inline");
    } else {
      $("#bar_padding_fixed_width_span").css("display", "none");
    }
  });
  // 画图事件
  $(".draw_trigger").change(function() {
    if (validate_basic_cfg()) {
      draw();
    }
  });

  // 标签页切换
  $("#tabs").tabs();

  // websocket
  var ws = new WebSocket("ws://" + location.hostname + ":" + location.port + "/ws/");
  setupWebsocketEvents(ws);

  $("#atr_submit").click(function(){
    var atrp_days = $("#atr_days_input").val();
    $.ajax({
      url: "/api/prioritized-stocks?atrp_days=" + encodeURIComponent(atrp_days),
      method: "GET",
      dataType: "json",
      success: function(resp) {
        atr.data($.map(resp, function(item){
          var rst = {
            code: item.code,
            display_name: item.display_name,
            msci: item.msci,
            hs300: item.hs300,
            atrp_days: item.atrp_days
          };
          if (item.atrp_max) {
            rst.atrp_max = item.atrp_max;
          }
          if (item.atrp_min) {
            rst.atrp_min = item.atrp_min;
          }
          if (item.atrp_avg) {
            rst.atrp_avg = item.atrp_avg;
          }
          return rst;
        }));
        atr.draw_table();
      },
      error: function(err) {
        console.log("ajax error on prioritized stocks with ATR metrics", err);
      }
  });
  });
});

